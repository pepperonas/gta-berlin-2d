// Neutrale Eingabe (keine Taste gedrückt) – für Demo-Welt und Tests.
export const idleInput = Object.freeze({
  moveX: 0, moveY: 0, steer: 0, throttle: 0, brake: 0, handbrake: false, sprint: false, walkSlow: false, horn: false,
  action: false, actionHeld: false, enterExit: false, ride: false, pause: false, mapToggle: false,
  menuUp: false, menuDown: false, menuLeft: false, menuRight: false, confirm: false, back: false,
  menuHover: null, menuPick: null,
  fire: false, firePressed: false, kick: false, reload: false, weaponNext: false, weaponPrev: false, weaponSlot: 0,
  aimX: 0, aimY: 0, aimWorld: null,
});
