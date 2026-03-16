const { chromium } = require('playwright');

(async () => {
  console.log('Launching browser...');
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    console.log('Opening...');
    await page.goto('http://localhost:8080');
    const html = await page.content();
    console.log('Page loaded. Checking HTML content...');

    if (!html.includes('<title>My website</title>'))
      throw new Error('Missing or incorrect <title>My website</title>');
    if (!html.includes('<div id="root">'))
      throw new Error('Missing root div');
    if (!html.includes('/dist/pages/index.js.js'))
      throw new Error('Missing main script');
    if (!html.includes('/dist/pages/index.js.css'))
      throw new Error('Missing stylesheet');
    if (!html.includes('lang="en"'))
      throw new Error('HTML lang attribute not set to en');

    console.log('All HTML checks passed!');
  } finally {
    await browser.close();
  }
})();