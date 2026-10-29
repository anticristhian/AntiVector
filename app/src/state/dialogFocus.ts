import { useEffect } from 'react';

/** Contain keyboard focus in a dialog and restore the invoking control on close. */
export function useDialogFocus(active: string | null) {
  useEffect(() => {
    if (!active) return;
    const dialog = document.querySelector<HTMLElement>('[role=dialog]');
    if (!dialog) return;
    const previous = document.activeElement as HTMLElement | null;
    const siblings = [...document.querySelectorAll<HTMLElement>('.application > :not(.modal-backdrop)')];
    const prior = siblings.map(element => element.inert);
    siblings.forEach(element => { element.inert = true; });
    dialog.tabIndex = -1;
    const controls = () => [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),[tabindex="0"]')];
    (controls()[0] ?? dialog).focus();
    const trap = (event: KeyboardEvent) => {
      if (event.key !== 'Tab') return;
      const items = controls();
      const index = items.indexOf(document.activeElement as HTMLElement);
      event.preventDefault();
      (items[(index + (event.shiftKey ? -1 : 1) + items.length) % items.length] ?? dialog).focus();
    };
    document.addEventListener('keydown', trap, true);
    return () => {
      document.removeEventListener('keydown', trap, true);
      siblings.forEach((element, index) => { element.inert = prior[index]; });
      if (previous?.isConnected) previous.focus();
    };
  }, [active]);
}
