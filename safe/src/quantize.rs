#![allow(non_snake_case)]

use crate::bootstrap::catch_panic_or;
use crate::ffi::{GifByteType, GifColorType, GIF_ERROR, GIF_OK};

const COLOR_ARRAY_SIZE: usize = 32768;
const BITS_PER_PRIM_COLOR: u8 = 5;
const MAX_PRIM_COLOR: u8 = 0x1f;
const GREEN_SHIFT: usize = BITS_PER_PRIM_COLOR as usize;
const RED_SHIFT: usize = 2 * GREEN_SHIFT;
const COLOR_MASK: usize = MAX_PRIM_COLOR as usize;

#[derive(Clone, Copy)]
struct QuantizedColorType {
    NewColorIndex: GifByteType,
    Count: i64,
    Pnext: Option<usize>,
}

impl Default for QuantizedColorType {
    fn default() -> Self {
        Self {
            NewColorIndex: 0,
            Count: 0,
            Pnext: None,
        }
    }
}

#[derive(Clone, Copy)]
struct NewColorMapType {
    RGBMin: [GifByteType; 3],
    RGBWidth: [GifByteType; 3],
    NumEntries: usize,
    Count: u64,
    QuantizedColors: Option<usize>,
}

impl Default for NewColorMapType {
    fn default() -> Self {
        Self {
            RGBMin: [0; 3],
            RGBWidth: [255; 3],
            NumEntries: 0,
            Count: 0,
            QuantizedColors: None,
        }
    }
}

fn color_component(index: usize, axis: usize) -> GifByteType {
    match axis {
        0 => (index >> RED_SHIFT) as GifByteType,
        1 => ((index >> GREEN_SHIFT) & COLOR_MASK) as GifByteType,
        _ => (index & COLOR_MASK) as GifByteType,
    }
}

fn sort_hash(index: usize, axis: usize) -> usize {
    usize::from(color_component(index, axis)) * 256 * 256
        + usize::from(color_component(index, (axis + 1) % 3)) * 256
        + usize::from(color_component(index, (axis + 2) % 3))
}

fn link_sorted(entries: &mut [QuantizedColorType], order: &[usize]) -> Option<usize> {
    if order.is_empty() {
        return None;
    }
    for pair in order.windows(2) {
        entries[pair[0]].Pnext = Some(pair[1]);
    }
    entries[*order.last().unwrap()].Pnext = None;
    Some(order[0])
}

fn subdiv_color_map(
    entries: &mut [QuantizedColorType],
    new_color_subdiv: &mut [NewColorMapType; 256],
    sort_array: &mut Vec<usize>,
    ColorMapSize: usize,
    NewColorMapSize: &mut usize,
) -> i32 {
    while ColorMapSize > *NewColorMapSize {
        let mut Index = 0usize;
        let mut SortRGBAxis = 0usize;
        let mut MaxSize = -1i32;

        for i in 0..*NewColorMapSize {
            if new_color_subdiv[i].NumEntries <= 1 {
                continue;
            }
            for j in 0..3 {
                let width = i32::from(new_color_subdiv[i].RGBWidth[j]);
                if width > MaxSize {
                    MaxSize = width;
                    Index = i;
                    SortRGBAxis = j;
                }
            }
        }

        if MaxSize == -1 {
            return GIF_OK;
        }

        sort_array.clear();
        let mut current = new_color_subdiv[Index].QuantizedColors;
        while let Some(index) = current {
            sort_array.push(index);
            current = entries[index].Pnext;
        }

        sort_array.sort_unstable_by_key(|&index| sort_hash(index, SortRGBAxis));
        new_color_subdiv[Index].QuantizedColors = link_sorted(entries, &sort_array);

        let mut quantized_color = sort_array[0];
        let mut Sum = (new_color_subdiv[Index].Count / 2) as i64 - entries[quantized_color].Count;
        let mut NumEntries = 1usize;
        let mut Count = entries[quantized_color].Count;

        while let Some(next) = entries[quantized_color].Pnext {
            Sum -= entries[next].Count;
            if Sum < 0 || entries[next].Pnext.is_none() {
                break;
            }
            quantized_color = next;
            NumEntries += 1;
            Count += entries[quantized_color].Count;
        }

        let next = match entries[quantized_color].Pnext {
            Some(next) => next,
            None => return GIF_OK,
        };

        let MaxColor =
            u32::from(color_component(quantized_color, SortRGBAxis)) << (8 - BITS_PER_PRIM_COLOR);
        let MinColor = u32::from(color_component(next, SortRGBAxis)) << (8 - BITS_PER_PRIM_COLOR);

        new_color_subdiv[*NewColorMapSize].QuantizedColors = Some(next);
        entries[quantized_color].Pnext = None;
        new_color_subdiv[*NewColorMapSize].Count = Count as u64;
        new_color_subdiv[Index].Count = new_color_subdiv[Index].Count.saturating_sub(Count as u64);
        new_color_subdiv[*NewColorMapSize].NumEntries =
            new_color_subdiv[Index].NumEntries - NumEntries;
        new_color_subdiv[Index].NumEntries = NumEntries;

        for j in 0..3 {
            new_color_subdiv[*NewColorMapSize].RGBMin[j] = new_color_subdiv[Index].RGBMin[j];
            new_color_subdiv[*NewColorMapSize].RGBWidth[j] = new_color_subdiv[Index].RGBWidth[j];
        }
        new_color_subdiv[*NewColorMapSize].RGBWidth[SortRGBAxis] =
            (u32::from(new_color_subdiv[*NewColorMapSize].RGBMin[SortRGBAxis])
                + u32::from(new_color_subdiv[*NewColorMapSize].RGBWidth[SortRGBAxis])
                - MinColor) as u8;
        new_color_subdiv[*NewColorMapSize].RGBMin[SortRGBAxis] = MinColor as u8;
        new_color_subdiv[Index].RGBWidth[SortRGBAxis] =
            (MaxColor - u32::from(new_color_subdiv[Index].RGBMin[SortRGBAxis])) as u8;

        *NewColorMapSize += 1;
    }

    GIF_OK
}

#[no_mangle]
// SAFETY: This C ABI entry point trusts the caller to uphold giflib pointer and callback preconditions.
pub unsafe extern "C" fn GifQuantizeBuffer(
    Width: u32,
    Height: u32,
    ColorMapSize: *mut i32,
    RedInput: *const GifByteType,
    GreenInput: *const GifByteType,
    BlueInput: *const GifByteType,
    OutputBuffer: *mut GifByteType,
    OutputColorMap: *mut GifColorType,
) -> i32 {
    // SAFETY: This touches raw C-owned giflib state under the function's FFI preconditions.
    catch_panic_or(GIF_ERROR, || unsafe {
        if ColorMapSize.is_null()
            || RedInput.is_null()
            || GreenInput.is_null()
            || BlueInput.is_null()
            || OutputBuffer.is_null()
            || OutputColorMap.is_null()
        {
            return GIF_ERROR;
        }

        let requested_color_map_size = match usize::try_from(*ColorMapSize) {
            Ok(size) if size > 0 && size <= 256 => size,
            _ => return GIF_ERROR,
        };
        let total_pixels = u64::from(Width) * u64::from(Height);
        let pixel_count = match usize::try_from(total_pixels) {
            Ok(pixel_count) => pixel_count,
            Err(_) => return GIF_ERROR,
        };

        let mut entries = vec![QuantizedColorType::default(); COLOR_ARRAY_SIZE];

        for i in 0..pixel_count {
            let index = ((usize::from(*RedInput.add(i) >> (8 - BITS_PER_PRIM_COLOR))) << RED_SHIFT)
                + ((usize::from(*GreenInput.add(i) >> (8 - BITS_PER_PRIM_COLOR))) << GREEN_SHIFT)
                + usize::from(*BlueInput.add(i) >> (8 - BITS_PER_PRIM_COLOR));
            entries[index].Count += 1;
        }

        let mut new_color_subdiv = [NewColorMapType::default(); 256];
        let first_non_empty = match entries.iter().position(|entry| entry.Count > 0) {
            Some(index) => index,
            None => return GIF_ERROR,
        };

        let mut quantized_color = first_non_empty;
        let mut num_of_entries = 1usize;
        new_color_subdiv[0].QuantizedColors = Some(first_non_empty);
        for i in (first_non_empty + 1)..COLOR_ARRAY_SIZE {
            if entries[i].Count > 0 {
                entries[quantized_color].Pnext = Some(i);
                quantized_color = i;
                num_of_entries += 1;
            }
        }
        entries[quantized_color].Pnext = None;
        new_color_subdiv[0].NumEntries = num_of_entries;
        new_color_subdiv[0].Count = total_pixels;

        let mut new_color_map_size = 1usize;
        let mut sort_array = Vec::with_capacity(COLOR_ARRAY_SIZE);
        if subdiv_color_map(
            &mut entries,
            &mut new_color_subdiv,
            &mut sort_array,
            requested_color_map_size,
            &mut new_color_map_size,
        ) != GIF_OK
        {
            return GIF_ERROR;
        }

        if new_color_map_size < requested_color_map_size {
            for index in new_color_map_size..requested_color_map_size {
                (*OutputColorMap.add(index)).Red = 0;
                (*OutputColorMap.add(index)).Green = 0;
                (*OutputColorMap.add(index)).Blue = 0;
            }
        }

        for i in 0..new_color_map_size {
            let j = new_color_subdiv[i].NumEntries;
            if j == 0 {
                continue;
            }

            let mut red = 0i64;
            let mut green = 0i64;
            let mut blue = 0i64;
            let mut current = new_color_subdiv[i].QuantizedColors;
            while let Some(index) = current {
                entries[index].NewColorIndex = i as u8;
                red += i64::from(color_component(index, 0));
                green += i64::from(color_component(index, 1));
                blue += i64::from(color_component(index, 2));
                current = entries[index].Pnext;
            }
            (*OutputColorMap.add(i)).Red =
                ((red << (8 - BITS_PER_PRIM_COLOR)) / i64::try_from(j).unwrap_or(1)) as u8;
            (*OutputColorMap.add(i)).Green =
                ((green << (8 - BITS_PER_PRIM_COLOR)) / i64::try_from(j).unwrap_or(1)) as u8;
            (*OutputColorMap.add(i)).Blue =
                ((blue << (8 - BITS_PER_PRIM_COLOR)) / i64::try_from(j).unwrap_or(1)) as u8;
        }

        let mut _MaxRGBError = [0i32; 3];
        for i in 0..pixel_count {
            let sampled_index = ((usize::from(*RedInput.add(i) >> (8 - BITS_PER_PRIM_COLOR)))
                << RED_SHIFT)
                + ((usize::from(*GreenInput.add(i) >> (8 - BITS_PER_PRIM_COLOR))) << GREEN_SHIFT)
                + usize::from(*BlueInput.add(i) >> (8 - BITS_PER_PRIM_COLOR));
            let mapped_index = usize::from(entries[sampled_index].NewColorIndex);
            *OutputBuffer.add(i) = mapped_index as u8;

            let color = &*OutputColorMap.add(mapped_index);
            _MaxRGBError[0] =
                _MaxRGBError[0].max((i32::from(color.Red) - i32::from(*RedInput.add(i))).abs());
            _MaxRGBError[1] =
                _MaxRGBError[1].max((i32::from(color.Green) - i32::from(*GreenInput.add(i))).abs());
            _MaxRGBError[2] =
                _MaxRGBError[2].max((i32::from(color.Blue) - i32::from(*BlueInput.add(i))).abs());
        }

        *ColorMapSize = new_color_map_size as i32;
        GIF_OK
    })
}
