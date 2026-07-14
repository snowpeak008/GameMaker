export function clear(element) {
  if (!element) {
    return;
  }
  while (element.firstChild) {
    element.firstChild.remove();
  }
}

export function el(tag, className, text) {
  const element = document.createElement(tag);
  if (className) {
    element.className = className;
  }
  if (text !== undefined) {
    element.textContent = text;
  }
  return element;
}
