export class MockElement {
  constructor(className = '') {
    this.className = className;
    this.textContent = '';
    this.classList = {
      add: (c) => { if (!this.className.includes(c)) this.className += ` ${c}`; },
      remove: (c) => { this.className = this.className.replace(c, '').trim(); },
      contains: (c) => this.className.includes(c)
    };
    this.elements = {};
  }
  querySelector(sel) {
    if (!this.elements[sel]) this.elements[sel] = new MockElement();
    return this.elements[sel];
  }
}
