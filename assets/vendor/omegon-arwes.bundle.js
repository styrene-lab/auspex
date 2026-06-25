// Minimal Arwes-facing custom-element bridge for the Dioxus Omegon Web mockup.
// This intentionally starts as a no-build, no-React bridge so the Dioxus SPA can
// ship contract-shaped dummy data immediately. Replace internals with real
// @arwes/react/@arwes/frames mounts once the JS bundling lane is added.

const template = document.createElement('template');
template.innerHTML = `<slot></slot>`;

class OmegonArwesElement extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) {
      const root = this.attachShadow({ mode: 'open' });
      const style = document.createElement('style');
      style.textContent = `
        :host { display: block; position: relative; box-sizing: border-box; }
        :host([hidden]) { display: none; }
        slot { position: relative; z-index: 1; }
      `;
      root.append(style, template.content.cloneNode(true));
    }
    this.dataset.arwesMounted = 'true';
  }
}

class OmegonArwesPanel extends OmegonArwesElement {
  connectedCallback() {
    super.connectedCallback();
    this.setAttribute('data-arwes-panel', this.getAttribute('variant') || 'default');
  }
}

class OmegonArwesStatusPill extends OmegonArwesElement {
  connectedCallback() {
    super.connectedCallback();
    this.setAttribute('data-arwes-status', this.getAttribute('status') || 'unknown');
  }
}

const define = (name, ctor) => {
  if (!customElements.get(name)) customElements.define(name, ctor);
};

define('omegon-arwes-app-shell', OmegonArwesElement);
define('omegon-arwes-bg', OmegonArwesElement);
define('omegon-arwes-panel', OmegonArwesPanel);
define('omegon-arwes-button', OmegonArwesElement);
define('omegon-arwes-status-pill', OmegonArwesStatusPill);
define('omegon-arwes-text', OmegonArwesElement);

window.omegonArwesBridge = {
  version: 'mock-bridge-0',
  mounted: true,
};
