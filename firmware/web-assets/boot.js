import init from '/axeos_web.js';
try {
  await init('/axeos_web_bg.wasm');
} catch (error) {
  const root = document.getElementById('app');
  root.replaceChildren();
  const message = document.createElement('p');
  message.className = 'boot';
  message.textContent = 'Impossible de charger AxeOS Rust. Rechargez la page dans un navigateur prenant en charge WebAssembly. Détail : ' + String(error);
  root.append(message);
}
