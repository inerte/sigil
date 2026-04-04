import './styles.css';
import { mountMinesweepApp } from './bridge';

const root = document.getElementById('root');

if (!root) {
  throw new Error('Missing #root element');
}

mountMinesweepApp(root);
