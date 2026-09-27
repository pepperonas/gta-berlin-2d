// Mauszeiger im Stil des Spiels (Gelb #ffd33d mit dunkler Kontur, wie Menü und Zielmarker) als SVG-Cursor für CSS.
// Kein DOM-Zugriff: liefert nur die Werte für canvas.style.cursor.
const Y = '#ffd33d', K = '#111';
const svg = (body) => `<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'>${body}</svg>`;
const ARROW = 'M4 3 L4 24.5 L9.6 19.3 L13.4 28 L17.4 26.3 L13.6 17.8 L21.2 17.8 Z';
const shadow = (d) => `<path d='${d}' fill='#000' opacity='.35' transform='translate(1.6 1.6)'/>`;

const SHAPES = {
  // normaler Zeiger
  arrow: [svg(`${shadow(ARROW)}<path d='${ARROW}' fill='${Y}' stroke='${K}' stroke-width='1.8' stroke-linejoin='round'/>`), 4, 3, 'default'],
  // über etwas Anklickbarem: heller Zeiger mit „Klick“-Strahlen an der Spitze
  hot: [svg(`${shadow(ARROW)}<path d='${ARROW}' fill='#fff' stroke='${K}' stroke-width='1.8' stroke-linejoin='round'/>`
    + `<g stroke='${K}' stroke-width='4.4' stroke-linecap='round'><path d='M9 3.4 L12.4 1.6'/><path d='M9.6 6.8 L13.4 6.8'/></g>`
    + `<g stroke='${Y}' stroke-width='2.2' stroke-linecap='round'><path d='M9 3.4 L12.4 1.6'/><path d='M9.6 6.8 L13.4 6.8'/></g>`), 4, 3, 'pointer'],
  // Stadtplan: Zielkreuz für den Teleport
  target: [svg(`<circle cx='16' cy='16' r='9' fill='none' stroke='${K}' stroke-width='5'/><circle cx='16' cy='16' r='9' fill='none' stroke='${Y}' stroke-width='2.4'/>`
    + `<g stroke='${K}' stroke-width='5' stroke-linecap='round'><path d='M16 2 V8'/><path d='M16 24 V30'/><path d='M2 16 H8'/><path d='M24 16 H30'/></g>`
    + `<g stroke='${Y}' stroke-width='2.2' stroke-linecap='round'><path d='M16 2 V8'/><path d='M16 24 V30'/><path d='M2 16 H8'/><path d='M24 16 H30'/></g>`
    + `<circle cx='16' cy='16' r='2.2' fill='${Y}' stroke='${K}' stroke-width='1.2'/>`), 16, 16, 'crosshair'],
  // Stadtplan ziehen
  move: [svg(`<g stroke='${K}' stroke-width='5' stroke-linecap='round' stroke-linejoin='round' fill='none'><path d='M16 4 V28 M4 16 H28 M12 8 L16 4 L20 8 M12 24 L16 28 L20 24 M8 12 L4 16 L8 20 M24 12 L28 16 L24 20'/></g>`
    + `<g stroke='${Y}' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' fill='none'><path d='M16 4 V28 M4 16 H28 M12 8 L16 4 L20 8 M12 24 L16 28 L20 24 M8 12 L4 16 L8 20 M24 12 L28 16 L24 20'/></g>`), 16, 16, 'move'],
};

export const CURSOR_KINDS = [...Object.keys(SHAPES), 'none'];

// CSS-Wert für canvas.style.cursor; 'none' blendet den Zeiger aus (beim Fahren ohne Mausbewegung).
export function cursorCss(kind) {
  if (kind === 'none') return 'none';
  const [s, hx, hy, fallback] = SHAPES[kind] ?? SHAPES.arrow;
  return `url("data:image/svg+xml,${encodeURIComponent(s)}") ${hx} ${hy}, ${fallback}`;
}

// Welcher Zeiger gerade passt: über welcher Fläche er steht (hit aus hud.hits), ob gezogen wird, ob gespielt wird
// und wie lange die Maus ruht (s).
export function cursorKind({ hit = null, dragging = false, playing = false, idle = 0 } = {}) {
  if (dragging) return 'move';
  if (hit?.kind === 'map') return 'target';
  if (hit) return 'hot';
  if (playing && idle > 2) return 'none';
  return 'arrow';
}
