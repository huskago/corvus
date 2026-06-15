use js_sys::{Function, Promise, Reflect};
use leptos::web_sys;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

pub async fn invoke<T, A>(cmd: &str, args: &A) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    A: Serialize,
{
    let args_js = serde_wasm_bindgen::to_value(args).map_err(|e| e.to_string())?;
    let result = raw_invoke(cmd, args_js).await?;
    serde_wasm_bindgen::from_value(result).map_err(|e| format!("Deserialisation: {e}"))
}

pub async fn invoke0<T: serde::de::DeserializeOwned>(cmd: &str) -> Result<T, String> {
    invoke(cmd, &serde_json::json!({})).await
}

async fn raw_invoke(cmd: &str, args: JsValue) -> Result<JsValue, String> {
    let win = web_sys::window().ok_or("window not found")?;

    let tauri = get_prop(&win, "__TAURI__")?;
    let core = get_prop(&tauri, "core")?;
    let func: Function = get_prop(&core, "invoke")?
        .dyn_into()
        .map_err(|_| "'invoke' is not a function")?;

    let promise: Promise = func
        .call2(&JsValue::UNDEFINED, &JsValue::from_str(cmd), &args)
        .map_err(|e| js_err(e))?
        .dyn_into()
        .map_err(|_| "invoke did not return a Promise")?;

    JsFuture::from(promise)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

fn get_prop(obj: &JsValue, key: &str) -> Result<JsValue, String> {
    Reflect::get(obj, &JsValue::from_str(key))
        .map_err(|_| format!("Property “{key}” not found"))
        .and_then(|v| {
            if v.is_undefined() {
                Err(format!("'{key}' is undefined"))
            } else {
                Ok(v)
            }
        })
}

fn js_err(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

pub fn listen<F>(event: &str, callback: F)
where
    F: Fn(JsValue) + 'static,
{
    let win = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let event = event.to_string();

    let Ok(tauri) = get_prop(&win, "__TAURI__") else {
        return;
    };
    let Ok(ev) = get_prop(&tauri, "event") else {
        return;
    };
    let Ok(func) = get_prop(&ev, "listen") else {
        return;
    };
    let Ok(func) = func.dyn_into::<Function>() else {
        return;
    };

    let closure = wasm_bindgen::closure::Closure::wrap(
        Box::new(move |e: JsValue| {
            let payload = Reflect::get(&e, &JsValue::from_str("payload"))
                .unwrap_or(JsValue::UNDEFINED);
            callback(payload);
        }) as Box<dyn Fn(JsValue)>
    );

    let _ = func.call2(
        &JsValue::UNDEFINED,
        &JsValue::from_str(&event),
        closure.as_ref(),
    );

    closure.forget();
}
