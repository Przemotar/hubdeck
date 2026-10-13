// HubDeck example plugin: counts presses on each control that uses it and shows the number.
// Needs Node 22 or later (built-in WebSocket); no packages. See docs/PLUGINS.md.

const { HUBDECK_PORT: port, HUBDECK_PLUGIN: plugin, HUBDECK_TOKEN: token } = process.env;
const ws = new WebSocket(`ws://127.0.0.1:${port}`);
const counts = new Map();
const send = (message) => ws.send(JSON.stringify(message));

const show = (context) => send({ event: "setTitle", context, title: String(counts.get(context)) });

ws.onopen = () => send({ event: "register", plugin, token });

ws.onmessage = ({ data }) => {
  const m = JSON.parse(data);
  switch (m.event) {
    case "willAppear":
      if (!counts.has(m.context)) counts.set(m.context, Number(m.settings?.start ?? 0));
      show(m.context);
      break;
    case "keyDown":
      counts.set(m.context, (counts.get(m.context) ?? 0) + 1);
      show(m.context);
      break;
    case "dialRotate":
      counts.set(m.context, (counts.get(m.context) ?? 0) + m.ticks);
      show(m.context);
      send({ event: "showFeedback", caption: `Count ${counts.get(m.context)}` });
      break;
    case "error":
      console.error(m.message);
      break;
  }
};

// HubDeck closed the connection: quit, it starts the plugin again next time
ws.onclose = () => process.exit(0);
