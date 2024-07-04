# konabg
A simple one file rust script to fetch backgrounds from konachan
## Usage
- `./konabg next` - loads the next background, sends it to swww and preloads the next one
- `./konabg prev` - same as `next` but goes backwards and doesnt preload
- `./konabg set <x>` - sets the background to the specified index
- `./konabg refresh` - loads the same image and sends it to swww

when `--lewds` is present, adds `rating:explicit` to the tags and keeps track of the posts, images, and count in a diffrent folder
## Installation

1. 
```sh
cargo build --release
```
2. Copy the executable from ./target/release/konabg to anywhere your heart desires