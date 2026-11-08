# `woff2`

`woff2` is a crate for converting WOFF2 font files to OpenType fonts.

## Examples

```rust
use woff2::decode::{convert_woff2_to_ttf, is_woff2};

let buffer = std::fs::read("src/test_resources/lato-v22-latin-regular.woff2").unwrap();
assert!(is_woff2(&buffer));
let ttf = convert_woff2_to_ttf(&mut std::io::Cursor::new(buffer)).unwrap();
// ... use `ttf` however you would use a loaded TTF file
```

### Command line utility

The `decoder` example is a simple command-line application to convert a WOFF2
font to OpenType format:

```shell
cargo run --example decoder input-filename.woff2 output-filename.ttf
```

## Unimplemented features / known issues

* WOFF2 fonts with `hmtx` transformations are not yet supported. It seems like these transformations are rare and we haven't found one yet. You can help by submitting a sample font that has them.
* WOFF (the original WOFF format) is not supported
* Converting OpenType to WOFF2 is not supported yet.

## Acknowledgements

* [WOFF File Format 2.0](https://www.w3.org/TR/WOFF2/)
* [Reference code](https://www.w3.org/TR/WOFF2/)
* [Allsorts](https://github.com/yeslogic/allsorts) We used some of the `glyf` decoding logic from Allsorts.

## License

Copyright 2022 Cimpress

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
