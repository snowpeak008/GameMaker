const returnTargets = new WeakMap();
const keydownHandlers = new WeakMap();

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

export function setModalVisible(modal, visible, preferredFocus = null) {
  if (!modal) {
    return;
  }
  if (visible) {
    const active = modal.ownerDocument?.activeElement;
    if (active && active !== modal.ownerDocument.body && !modal.contains(active)) {
      returnTargets.set(modal, active);
    }
    detachModalKeyboardHandler(modal);
    modal.hidden = false;
    modal.removeAttribute("aria-hidden");
    const keydown = (event) => handleModalKeydown(event, modal);
    modal.addEventListener("keydown", keydown);
    keydownHandlers.set(modal, keydown);
    const schedule = modal.ownerDocument?.defaultView?.queueMicrotask ?? globalThis.queueMicrotask;
    schedule?.(() => {
      const target = preferredFocus ?? focusableElements(modal)[0];
      target?.focus?.();
    });
    return;
  }
  detachModalKeyboardHandler(modal);
  modal.hidden = true;
  modal.setAttribute("aria-hidden", "true");
  const target = returnTargets.get(modal);
  returnTargets.delete(modal);
  if (target?.isConnected) {
    target.focus?.();
  }
}

function handleModalKeydown(event, modal) {
  if (event.key === "Escape") {
    const cancel = focusableElements(modal).find((element) =>
      element.matches?.('.modal-footer [data-action^="cancel-"]'),
    );
    if (cancel) {
      event.preventDefault();
      event.stopPropagation();
      cancel.click();
    }
    return;
  }
  if (event.key !== "Tab") {
    return;
  }
  const focusable = focusableElements(modal);
  if (focusable.length === 0) {
    event.preventDefault();
    return;
  }
  const active = modal.ownerDocument?.activeElement;
  const index = focusable.indexOf(active);
  const nextIndex = event.shiftKey
    ? index <= 0
      ? focusable.length - 1
      : index - 1
    : index < 0 || index === focusable.length - 1
      ? 0
      : index + 1;
  if (index < 0 || (event.shiftKey && index === 0) || (!event.shiftKey && index === focusable.length - 1)) {
    event.preventDefault();
    focusable[nextIndex].focus();
  }
}

function focusableElements(modal) {
  return [...modal.querySelectorAll(FOCUSABLE_SELECTOR)].filter((element) => {
    if (
      element.tabIndex < 0 ||
      element.hidden ||
      element.closest("[hidden]") ||
      element.getAttribute("aria-hidden") === "true"
    ) {
      return false;
    }
    const windowRef = modal.ownerDocument?.defaultView;
    const style = windowRef?.getComputedStyle?.(element);
    return !style || (style.display !== "none" && style.visibility !== "hidden");
  });
}

function detachModalKeyboardHandler(modal) {
  const keydown = keydownHandlers.get(modal);
  if (keydown) {
    modal.removeEventListener("keydown", keydown);
    keydownHandlers.delete(modal);
  }
}
