# LG WebOs Client 0.3.1

[![Build Status](https://travis-ci.com/kziemianek/lg-webos-client.svg?branch=main)](https://travis-ci.com/kziemianek/lg-webos-client)


Simple LG webOS client written purerly in Rust.
Inspired by [lgtv.js](https://github.com/msloth/lgtv.js)

## Supported commands

* Create toast
* Open browser
* Turn off
* Set channel
* Set input
* Set mute
* Set volume
* Get channel list
* Get current channel
* Open channel
* Get external input list
* Switch input
* Is muted
* Get volume
* Play media
* Stop media
* Pause media
* Rewind media
* Forward media
* Channel up
* Channel down
* Turn 3d on
* Turn 3d off
* Get services list
* Launch an app

## Example

Add to `Cargo.toml`

```toml
[dependencies]
lg-webos-client = "0.3.1"
tokio = { version = "1.2.0", default-features = false, features = ["full"] }
```

And then use the following snippet

```rust
use lg_webos_client::client::*;
use lg_webos_client::command::Command;

#[tokio::main]
async fn main() {
    env_logger::init();
    // Note: We must specify the ws protocol, and if we do not have the key, we pass None.
    let config = WebOsClientConfig::new("ws://192.168.1.62:3000/", None);
    
    let mut client = WebosClient::new(config).await.unwrap();
    // Registration successful, store the key for later.
    let key = client.key;
    let resp = client.send_command(Command::GetChannelList).await.unwrap();
    println!("Got response {:?}", resp.payload);

    // We can reuse the key.
    let config = WebOsClientConfig::new("ws://192.168.1.62:3000/", key);
    // ...
}
```

The code above simply connects to tv in local network and after a successful registration lists all channels.
Note that we must specify the `ws` protocol.  If we have a key, we can pass that in the config, and the 
client will use said key.  If no key is specified, the client will output a key generated from the device.

# Contributors
* kziemianek
* GT3CH1
