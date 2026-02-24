import { main } from './factorial';

if (typeof main !== 'function') {
  console.error('Error: No main() function found in ../examples/factorial.sigil');
  console.error('Add a main() function to make this program runnable.');
  process.exit(1);
}

// Call main and handle the result
const result = main();

// If main returns a value (not Unit/undefined), show it
if (result !== undefined) {
  console.log(result);
}
