**Сборка:**
```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1
# артефакт: target/wasm32-wasip1/release/message.wasm
```

**Размещение в git-репо настроек:** 
скопировать `message.wasm` в 
`wasm/function_settings/message.wasm`, 
в JSON-настройке указать:
```json
{ "key": "...", "module": ["echo.wasm"] }
```
