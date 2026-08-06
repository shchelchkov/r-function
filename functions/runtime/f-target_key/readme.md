**Сборка:**
```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1
# артефакт: target/wasm32-wasip1/release/wasm-function.wasm
```

**Размещение в git-репо настроек:** 
скопировать `wasm-function.wasm` в 
`wasm/function_settings/wasm-function.wasm`, 
в JSON-настройке указать:
```json
{ "key": "...", "module": ["echo.wasm"] }
```
