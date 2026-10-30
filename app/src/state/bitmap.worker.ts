// Decode and prepare the full-resolution source away from the UI thread.
export {};
self.onmessage = async (event: MessageEvent<string>) => {
  try {
    const bytes = Uint8Array.from(atob(event.data), c => c.charCodeAt(0));
    const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
    self.postMessage(bitmap, { transfer: [bitmap] });
  } catch {
    self.postMessage({ error: true }, {});
  }
};
