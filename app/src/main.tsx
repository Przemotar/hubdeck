import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

const render = () =>
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );

// `?demo` in a plain browser: a mocked engine with sample modules and profiles (src/demo)
if (!("__TAURI_INTERNALS__" in window) && new URLSearchParams(location.search).has("demo")) {
  import("./demo").then((demo) => {
    demo.installDemo();
    render();
  });
} else {
  render();
}
