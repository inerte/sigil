import './styles.css';
import { mount2048App } from './bridge';

const root = document.getElementById('root');

if (!root) {
  throw new Error('Missing #root element');
}

mount2048App(root);
