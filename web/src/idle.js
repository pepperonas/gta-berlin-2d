// Neutrale Eingabe (keine Taste gedrückt) – für Demo-Welt und Tests.
export const idleInput = Object.freeze({
  moveX: 0, moveY: 0, steer: 0, throttle: 0, brake: 0, handbrake: false, sprint: false, horn: false,
  action: false, actionHeld: false, enterExit: false, pause: false, mapToggle: false,
  menuUp: false, menuDown: false, menuLeft: false, menuRight: false, confirm: false, back: false,
  menuHover: null, menuPick: null,
});
