import { main } from './effect-demo';

if (typeof main !== 'function') {
  console.error('Error: No main() function found in ../examples/effect-demo.sigil');
  console.error('Add a main() function to make this program runnable.');
  process.exit(1);
}

// Call main and handle the result (all Sigil functions are async)
const result = await main();

// If main returns a value (not Unit/undefined), show it
if (result !== undefined) {
  console.log(result);
}
