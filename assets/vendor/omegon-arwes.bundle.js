// Minimal Arwes-facing custom-element bridge for the Dioxus Omegon Web mockup.
// This first-look bridge intentionally avoids a JS build step: it gives Dioxus
// stable custom elements now, while keeping the internals replaceable with real
// @arwes/react/@arwes/frames mounts once the bundling lane is added.

const template = document.createElement('template');
template.innerHTML = `<slot></slot>`;

const bridgeStyles = `
  :host {
    --arwes-line: rgba(100, 244, 255, 0.48);
    --arwes-line-soft: rgba(100, 244, 255, 0.18);
    --arwes-warn: rgba(255, 191, 102, 0.82);
    --arwes-danger: rgba(255, 116, 116, 0.82);
    --arwes-ok: rgba(110, 231, 168, 0.82);
    display: block;
    position: relative;
    box-sizing: border-box;
  }

  :host([hidden]) { display: none; }

  :host::before,
  :host::after {
    content: '';
    position: absolute;
    pointer-events: none;
    z-index: 0;
  }

  slot {
    position: relative;
    z-index: 1;
  }

  :host([data-arwes-panel])::before {
    inset: -1px;
    border: 1px solid var(--arwes-line);
    clip-path: polygon(18px 0, 100% 0, 100% calc(100% - 18px), calc(100% - 18px) 100%, 0 100%, 0 18px);
    opacity: 0.72;
    filter: drop-shadow(0 0 10px var(--arwes-line-soft));
  }

  :host([data-arwes-panel])::after {
    display: none;
  }

  :host([data-arwes-panel='primary'])::before,
  :host([data-arwes-panel='composer'])::before {
    opacity: 0.9;
  }

  :host([data-arwes-panel='modal'])::before {
    --arwes-line: rgba(169, 140, 255, 0.72);
  }

  :host([data-arwes-panel='runtime'])::before {
    --arwes-line: rgba(110, 231, 168, 0.62);
  }

  :host([data-arwes-panel='workbench'])::before {
    --arwes-line: rgba(100, 244, 255, 0.62);
  }

  :host([data-arwes-panel='memory'])::before {
    --arwes-line: rgba(169, 140, 255, 0.62);
  }

  :host([data-arwes-status]) {
    display: inline-block;
  }

  :host([data-arwes-status])::before {
    width: 7px;
    height: 7px;
    left: 9px;
    top: 50%;
    transform: translateY(-50%);
    border-radius: 999px;
    background: var(--arwes-ok);
    box-shadow: 0 0 10px var(--arwes-ok);
  }

  :host([data-arwes-status='waiting'])::before {
    background: var(--arwes-warn);
    box-shadow: 0 0 12px var(--arwes-warn);
    animation: omegon-arwes-pulse 1.4s ease-in-out infinite;
  }

  :host([data-arwes-status='denied'])::before,
  :host([data-arwes-status='degraded'])::before {
    background: var(--arwes-danger);
    box-shadow: 0 0 12px var(--arwes-danger);
  }

  :host([data-arwes-text]) slot {
    text-shadow: 0 0 14px rgba(100, 244, 255, 0.34);
  }

  :host([data-arwes-bg]) {
    overflow: hidden;
  }

  :host([data-arwes-bg])::before {
    inset: 0;
    background:
      linear-gradient(120deg, transparent 0 44%, rgba(100, 244, 255, 0.08) 45%, transparent 46% 100%),
      radial-gradient(circle at 20% 20%, rgba(100, 244, 255, 0.12), transparent 26%),
      radial-gradient(circle at 80% 0%, rgba(169, 140, 255, 0.1), transparent 24%);
    animation: omegon-arwes-drift 12s linear infinite;
  }

  :host([data-arwes-bg])::after {
    inset: 0;
    background-image: repeating-linear-gradient(0deg, rgba(255, 255, 255, 0.035) 0 1px, transparent 1px 5px);
    mix-blend-mode: screen;
    opacity: 0.32;
  }

  @keyframes omegon-arwes-pulse {
    0%, 100% { opacity: 0.48; transform: translateY(-50%) scale(0.9); }
    50% { opacity: 1; transform: translateY(-50%) scale(1.22); }
  }

  @keyframes omegon-arwes-drift {
    from { transform: translate3d(-2%, -1%, 0); }
    to { transform: translate3d(2%, 1%, 0); }
  }
`;

class OmegonArwesElement extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) {
      const root = this.attachShadow({ mode: 'open' });
      const style = document.createElement('style');
      style.textContent = bridgeStyles;
      root.append(style, template.content.cloneNode(true));
    }
    this.dataset.arwesMounted = 'true';
  }
}

class OmegonArwesBg extends OmegonArwesElement {
  connectedCallback() {
    super.connectedCallback();
    this.setAttribute('data-arwes-bg', 'true');
  }
}

class OmegonArwesPanel extends OmegonArwesElement {
  static get observedAttributes() { return ['variant']; }

  connectedCallback() {
    super.connectedCallback();
    this.syncVariant();
  }

  attributeChangedCallback() {
    this.syncVariant();
  }

  syncVariant() {
    this.setAttribute('data-arwes-panel', this.getAttribute('variant') || 'default');
  }
}

class OmegonArwesStatusPill extends OmegonArwesElement {
  static get observedAttributes() { return ['status']; }

  connectedCallback() {
    super.connectedCallback();
    this.syncStatus();
  }

  attributeChangedCallback() {
    this.syncStatus();
  }

  syncStatus() {
    this.setAttribute('data-arwes-status', this.getAttribute('status') || 'unknown');
  }
}

class OmegonArwesText extends OmegonArwesElement {
  connectedCallback() {
    super.connectedCallback();
    this.setAttribute('data-arwes-text', 'true');
  }
}

const define = (name, ctor) => {
  if (!customElements.get(name)) customElements.define(name, ctor);
};

define('omegon-arwes-app-shell', OmegonArwesElement);
define('omegon-arwes-bg', OmegonArwesBg);
define('omegon-arwes-panel', OmegonArwesPanel);
define('omegon-arwes-button', OmegonArwesElement);
define('omegon-arwes-status-pill', OmegonArwesStatusPill);
define('omegon-arwes-text', OmegonArwesText);

const installInteractionEffects = () => {
  if (window.omegonArwesBridge?.effectsInstalled) return;

  let audioContext = null;
  const blip = (frequency = 880, duration = 0.045) => {
    try {
      audioContext ??= new AudioContext();
      const oscillator = audioContext.createOscillator();
      const gain = audioContext.createGain();
      oscillator.type = 'triangle';
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.0001, audioContext.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.035, audioContext.currentTime + 0.008);
      gain.gain.exponentialRampToValueAtTime(0.0001, audioContext.currentTime + duration);
      oscillator.connect(gain);
      gain.connect(audioContext.destination);
      oscillator.start();
      oscillator.stop(audioContext.currentTime + duration);
    } catch (_) {
      // Audio is progressive enhancement; browsers may block it before gesture.
    }
  };

  document.addEventListener('pointerover', (event) => {
    if (event.target instanceof Element && event.target.closest('button')) blip(1320, 0.025);
  }, { passive: true });

  document.addEventListener('click', (event) => {
    if (event.target instanceof Element && event.target.closest('button')) blip(620, 0.06);
  }, { passive: true });

  window.omegonArwesBridge.effectsInstalled = true;
};

window.omegonArwesBridge = {
  version: 'mock-bridge-2',
  mounted: true,
  effectsInstalled: false,
  elements: [
    'omegon-arwes-app-shell',
    'omegon-arwes-bg',
    'omegon-arwes-panel',
    'omegon-arwes-button',
    'omegon-arwes-status-pill',
    'omegon-arwes-text',
  ],
};

installInteractionEffects();
