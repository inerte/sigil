import './styles.css';
import { mountTodoApp } from './bridge';

const root = document.getElementById('root');

if (!root) {
  throw new Error('Missing #root element');
}

mountTodoApp(root);
