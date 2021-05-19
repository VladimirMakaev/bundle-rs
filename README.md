# bundle-rs
The main purpose of this crate is to bundle Rust projects in a single file.
The target use case for this is submitting code to https://www.codingame.com/ where their sync tool requires the code to be in a single file which is very inconvenient when working on a complex algorithm
## Example:

Let consider we have source code in 2 files

*main.rs*
```rust
mod common;

use common:*;

pub fn main() {
    let result = reusable_function();

    match result {
        MyType::Value1 => todo!(),
        MyType::Value2 => todo!()
    }
    /*
    Program entry point .....
    .....
    ...
    */
}
```
*common.rs*
```rust
pub enum MyType {
    Value1,
    Value2
}

pub fn reusable_function() -> MyType {
    todo!()
}
/*....*/
```

This crate allows to bundle this project consisting of 2 files into functionally equivalent single file that looks like this:
*output.rs*
```rust
mod common {
    pub enum MyType {
        Value1,
        Value2
    }

    pub fn reusable_function() -> MyType {
        todo!()
    }
}

use common:*;

pub fn main() {
    let result = reusable_function();

    match result {
        MyType::Value1 => todo!(),
        MyType::Value2 => todo!()
    }
    /*
    Program entry point .....
    .....
    ...
    */
}
```
Which in it's turn is linked to Coding Game Sync App that uploads it to the Coding Game platform.

## Usage
Add *build.rs* file your cargo project with the following content:
```rust
use bundle_rs::{Bundle, ModuleFileSystem};

fn main() -> std::io::Result<()> {
    let mut bundle = Bundle::new("main", ModuleFileSystem::new(vec!["./src"]));
    bundle.load()?;
    return bundle.write(&mut std::fs::File::create("./dist/singlefile.rs")?);
}

```
(_Note: you can link to any executable you want not specifically main _ )
Add the following to the *Cargo.toml* of your project
```toml
[package]
build = "build/build.rs" #path to build.rs where you decided to put it
#....

[build-dependencies]
bundle-rs = {git = "https://github.com/VladimirMakaev/bundle-rs.git", branch = "main"}
```

Now when you build your project a single file with bundled source code will be placed to *./dist/singlefile.rs*

