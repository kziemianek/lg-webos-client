# LG webos client

[![Build Status](https://travis-ci.com/kziemianek/lg-webos-client.svg?branch=main)](https://travis-ci.com/kziemianek/lg-webos-client)


Simple LG webOS client written purerly in Rust.
Inspired by [lgtv.js](https://github.com/msloth/lgtv.js)

## Supported commands

* create toast
* open browser
* turn off
* set channel
* set input
* set mute
* set volume

## Example

Add to `Cargo.toml`

```toml
[dependencies]
lg-webos-client = "0.1.0"
tokio = { version = "1.2.0", default-features = false, features = ["full"] }
```

And then write code

```rust
#[tokio::main]
async fn main() {
    let client = lg_webos_client::WebosClient::new("ws://192.168.1.62:3000/").await;
    // wait for registration...
    std::thread::sleep(std::time::Duration::from_millis(3000));
    client
        .send_command(lg_webos_client::Command::SetVolume(20)).await;
}
```

The code above simply connects to tv in local network and after successfull registration it sets volume to 20.