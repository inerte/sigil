import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: process.env.SIGIL_FLASHCARDS_BASE ?? '/',
  root: 'web',
  plugins: [react()],
});
