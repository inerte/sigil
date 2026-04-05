import './styles.css';
import { mountFlashcardsApp } from './bridge';

const root = document.getElementById('root');

if (!root) {
  throw new Error('Missing #root element');
}

mountFlashcardsApp(root);
