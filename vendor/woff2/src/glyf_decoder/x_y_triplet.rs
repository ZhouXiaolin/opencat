// Copied from the Allsorts Rust package
// https://github.com/yeslogic/allsorts/blob/master/src/woff2/lut.rs
//
// Copyright 2019 YesLogic Pty. Ltd. <info@yeslogic.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub struct XYTriplet {
    pub x_is_negative: bool,
    pub y_is_negative: bool,
    pub byte_count: u8,
    pub x_bits: u8,
    pub y_bits: u8,
    pub delta_x: u16,
    pub delta_y: u16,
}

impl XYTriplet {
    pub fn dx(&self, data: u32) -> i16 {
        let mask = (1u32 << self.x_bits) - 1;
        let shift = (self.byte_count * 8) - self.x_bits;
        let dx = ((data >> shift) & mask) + u32::from(self.delta_x);

        if self.x_is_negative {
            -(dx as i16)
        } else {
            dx as i16
        }
    }

    pub fn dy(&self, data: u32) -> i16 {
        let mask = (1u32 << self.y_bits) - 1;
        let shift = (self.byte_count * 8) - self.x_bits - self.y_bits;
        let dy = ((data >> shift) & mask) + u32::from(self.delta_y);

        if self.y_is_negative {
            -(dy as i16)
        } else {
            dy as i16
        }
    }
}

// Lookup table for decoding transformed glyf table point coordinates
// https://www.w3.org/TR/WOFF2/#glyf_table_format
#[rustfmt::skip]
pub static COORD_LUT: [XYTriplet; 128] = [
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 0,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 0,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 256,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 256,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 512,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 512,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 768,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 768,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 1024, x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 0,  y_bits: 8,  delta_x: 0,    delta_y: 1024, x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 0,    delta_y: 0,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 0,    delta_y: 0,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 256,  delta_y: 0,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 256,  delta_y: 0,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 512,  delta_y: 0,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 512,  delta_y: 0,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 768,  delta_y: 0,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 768,  delta_y: 0,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 1024, delta_y: 0,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 8,  y_bits: 0,  delta_x: 1024, delta_y: 0,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 1,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 1,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 1,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 1,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 17,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 17,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 17,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 17,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 33,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 33,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 33,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 33,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 49,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 49,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 49,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 1,    delta_y: 49,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 1,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 1,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 1,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 1,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 17,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 17,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 17,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 17,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 33,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 33,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 33,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 33,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 49,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 49,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 49,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 17,   delta_y: 49,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 1,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 1,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 1,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 1,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 17,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 17,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 17,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 17,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 33,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 33,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 33,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 33,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 49,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 49,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 49,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 33,   delta_y: 49,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 1,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 1,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 1,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 1,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 17,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 17,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 17,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 17,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 33,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 33,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 33,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 33,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 49,   x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 49,   x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 49,   x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 1, x_bits: 4,  y_bits: 4,  delta_x: 49,   delta_y: 49,   x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 1,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 1,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 1,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 1,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 257,  x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 257,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 257,  x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 257,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 513,  x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 513,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 513,  x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 1,    delta_y: 513,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 1,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 1,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 1,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 1,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 257,  x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 257,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 257,  x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 257,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 513,  x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 513,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 513,  x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 257,  delta_y: 513,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 1,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 1,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 1,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 1,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 257,  x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 257,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 257,  x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 257,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 513,  x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 513,  x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 513,  x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 2, x_bits: 8,  y_bits: 8,  delta_x: 513,  delta_y: 513,  x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 3, x_bits: 12, y_bits: 12, delta_x: 0,    delta_y: 0,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 3, x_bits: 12, y_bits: 12, delta_x: 0,    delta_y: 0,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 3, x_bits: 12, y_bits: 12, delta_x: 0,    delta_y: 0,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 3, x_bits: 12, y_bits: 12, delta_x: 0,    delta_y: 0,    x_is_negative: false, y_is_negative: false },
    XYTriplet { byte_count: 4, x_bits: 16, y_bits: 16, delta_x: 0,    delta_y: 0,    x_is_negative: true,  y_is_negative: true  },
    XYTriplet { byte_count: 4, x_bits: 16, y_bits: 16, delta_x: 0,    delta_y: 0,    x_is_negative: false, y_is_negative: true  },
    XYTriplet { byte_count: 4, x_bits: 16, y_bits: 16, delta_x: 0,    delta_y: 0,    x_is_negative: true,  y_is_negative: false },
    XYTriplet { byte_count: 4, x_bits: 16, y_bits: 16, delta_x: 0,    delta_y: 0,    x_is_negative: false, y_is_negative: false },
];
