import { useEffect, useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    setGreetMsg(await invoke("greet", { name }));
  }

  useEffect(() => {}, []);

  return (
    <div className="container">
      <h1>Welcome to Tauri!</h1>

      <div className="row">
        <a href="https://vitejs.dev" target="_blank">
          <img src="/vite.svg" className="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://reactjs.org" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>

      <p>Click on the Tauri, Vite, and React logos to learn more.</p>

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button
          className="rounded-full bg-blue-500 text-white p-2 hover:bg-orange-500"
          onPointerEnter={(event) => {
            invoke("ignore_mouse_events", {
              ignore: false,
              forward: false,
            });
            console.log("指针移入", event.clientX, event.clientY);
          }}
          onPointerLeave={(event) => {
            invoke("ignore_mouse_events", {
              ignore: true,
              forward: true,
            });
            console.log("指针移出", event.clientX, event.clientY);
          }}
          type="submit"
        >
          Greet
        </button>
      </form>

      <p>{greetMsg}</p>
    </div>
  );
}

export default App;
