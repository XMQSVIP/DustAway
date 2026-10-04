import '@testing-library/jest-dom/vitest';

// jsdom has no layout scrolling; native tests cover the actual position.
HTMLElement.prototype.scrollIntoView = function () {};
