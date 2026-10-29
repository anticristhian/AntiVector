import { create } from 'zustand';
import en from './en.json';
import pt from './pt-BR.json';

export type Language = 'pt-BR' | 'en';
type Theme = 'light' | 'dark' | 'system';
function saved(key: string): string | null {
  try { return localStorage.getItem(`antivector.${key}`); } catch { return null; }
}
function persist(key: string, value: string) {
  try { localStorage.setItem(`antivector.${key}`, value); } catch { /* Preferences never block work. */ }
}
const initialLanguage: Language = saved('language') === 'en' ? 'en' : 'pt-BR';
const storedTheme = saved('theme');
const initialTheme: Theme = storedTheme === 'light' || storedTheme === 'dark' ? storedTheme : 'system';
export const usePreferences = create<{
  language: Language; theme: Theme;
  setLanguage: (language: Language) => void; setTheme: (theme: Theme) => void;
}>((set) => ({
  language: initialLanguage, theme: initialTheme,
  setLanguage: language => { persist('language', language); set({ language }); },
  setTheme: theme => { persist('theme', theme); set({ theme }); },
}));

/** Plain JSON dictionaries, original text as a stable lookup key. No network. */
export function t(key: string): string {
  const dictionary: Record<string, string> = usePreferences.getState().language === 'en' ? en : pt;
  return dictionary[key] ?? key;
}
export function locale(): Language { return usePreferences.getState().language; }
