const { chromium } = require('playwright');

const BASE_URL = 'http://localhost:8080';

let browser;
let passed = 0;
let failed = 0;

async function assert(condition, message) {
    if (condition) {
        console.log(`  [PASS] ${message}`);
        passed++;
    } else {
        console.error(`  [FAIL] ${message}`);
        failed++;
    }
}

async function runTests() {
    console.log('Launching browser...');
    browser = await chromium.launch();

    await testStaticStructure();
    await testIndexContent();
    await testHomeContent();
    await testBlogContent();
    await testNavigation();
    await testFooterCounter();
    await testHomeCounter();
    await testApiEndpoint();

    await browser.close();

    console.log(`\n=== Results: ${passed} passed, ${failed} failed ===`);
    if (failed > 0) process.exit(1);
}

// --- Static HTML structure (checked before JS runs) ---
async function testStaticStructure() {
    console.log('\n[Suite] Static HTML structure');
    const page = await browser.newPage();
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded', timeout: 30000 });

    const html = await page.content();

    await assert(html.includes('<title>My website</title>'), 'Has correct <title>My website</title>');
    await assert(html.includes('lang="en"'), 'Has lang="en" attribute on <html>');
    await assert(html.includes('id="root"'), 'Has #root element');
    await assert(html.includes('/dist/pages/index.js.js'), 'Has page JS bundle reference');
    await assert(html.includes('/dist/pages/index.js.css'), 'Has page CSS bundle reference');

    await page.close();
}

// --- Index page: dynamic content (works for SSR and CSR) ---
async function testIndexContent() {
    console.log('\n[Suite] Index page content (/)');
    const page = await browser.newPage();
    await page.goto(BASE_URL, { waitUntil: 'networkidle', timeout: 30000 });

    // Wait for React to render (handles both SSR hydration and CSR)
    await page.waitForSelector('h1', { timeout: 15000 });

    const h1Text = await page.locator('h1').first().innerText();
    await assert(h1Text.includes('Hello from index page'), `Index h1 rendered: "${h1Text}"`);

    // Navigation links
    await assert(
        await page.locator('a[href="/home"]').count() > 0,
        'Nav link to /home is present'
    );
    await assert(
        await page.locator('a[href="/blog"]').count() > 0,
        'Nav link to /blog is present'
    );

    // Footer
    await page.waitForSelector('footer', { timeout: 10000 });
    const footerText = await page.locator('footer').innerText();
    await assert(footerText.includes('This is a footer'), `Footer rendered: "${footerText.split('\n')[0]}"`);

    await page.close();
}

// --- Home page: heading + initial counter value ---
async function testHomeContent() {
    console.log('\n[Suite] Home page content (/home)');
    const page = await browser.newPage();
    await page.goto(`${BASE_URL}/home`, { waitUntil: 'networkidle', timeout: 30000 });

    await page.waitForSelector('h1', { timeout: 15000 });

    const headingText = await page.locator('div').filter({ hasText: /This is a simple home page/ }).first().innerText();
    await assert(
        headingText.includes('This is a simple home page contains a counter'),
        `Home heading rendered: "${headingText.substring(0, 50)}"`
    );

    // Counter initial value
    const counterText = await page.locator('h1').first().innerText();
    await assert(counterText.trim() === '0', `Home counter initial value is 0 (got: "${counterText.trim()}")`);

    await page.close();
}

// --- Blog page: dynamic content ---
async function testBlogContent() {
    console.log('\n[Suite] Blog page content (/blog)');
    const page = await browser.newPage();
    await page.goto(`${BASE_URL}/blog`, { waitUntil: 'networkidle', timeout: 30000 });

    await page.waitForSelector('div', { timeout: 15000 });

    const bodyText = await page.locator('body').innerText();
    await assert(
        bodyText.includes('This is a cool blog'),
        `Blog content rendered: "${bodyText.substring(0, 50)}"`
    );

    await page.close();
}

// --- Navigation: clicking header links changes page content ---
async function testNavigation() {
    console.log('\n[Suite] Navigation');
    const page = await browser.newPage();
    await page.goto(BASE_URL, { waitUntil: 'networkidle', timeout: 30000 });
    await page.waitForSelector('a[href="/home"]', { timeout: 10000 });

    // Navigate to /home
    await Promise.all([
        page.waitForLoadState('networkidle'),
        page.click('a[href="/home"]'),
    ]);
    const homeBodyText = await page.locator('body').innerText();
    await assert(
        homeBodyText.includes('This is a simple home page contains a counter'),
        'Navigating to /home renders home content'
    );

    // Navigate to /blog
    await page.waitForSelector('a[href="/blog"]', { timeout: 10000 });
    await Promise.all([
        page.waitForLoadState('networkidle'),
        page.click('a[href="/blog"]'),
    ]);
    const blogBodyText = await page.locator('body').innerText();
    await assert(
        blogBodyText.includes('This is a cool blog'),
        'Navigating to /blog renders blog content'
    );

    // Navigate back to index
    await page.waitForSelector('a[href="/"]', { timeout: 10000 });
    await Promise.all([
        page.waitForLoadState('networkidle'),
        page.click('a[href="/"]'),
    ]);
    const indexBodyText = await page.locator('body').innerText();
    await assert(
        indexBodyText.includes('Hello from index page'),
        'Navigating to / renders index content'
    );

    await page.close();
}

// --- Footer counter: client-side interactivity ---
async function testFooterCounter() {
    console.log('\n[Suite] Footer counter (client-side interactivity)');
    const page = await browser.newPage();
    await page.goto(BASE_URL, { waitUntil: 'networkidle', timeout: 30000 });

    const footerButton = page.locator('footer button');
    await footerButton.waitFor({ state: 'visible', timeout: 10000 });

    const initialText = await footerButton.innerText();
    await assert(initialText.includes('0'), `Footer counter starts at 0 (got: "${initialText.trim()}")`);

    await footerButton.click();
    const afterOne = await footerButton.innerText();
    await assert(afterOne.includes('1'), `Footer counter shows 1 after 1 click (got: "${afterOne.trim()}")`);

    await footerButton.click();
    await footerButton.click();
    const afterThree = await footerButton.innerText();
    await assert(afterThree.includes('3'), `Footer counter shows 3 after 3 clicks (got: "${afterThree.trim()}")`);

    await page.close();
}

// --- Home counter: client-side interactivity ---
async function testHomeCounter() {
    console.log('\n[Suite] Home page counter (client-side interactivity)');
    const page = await browser.newPage();
    await page.goto(`${BASE_URL}/home`, { waitUntil: 'networkidle', timeout: 30000 });

    const counterEl = page.locator('h1').first();
    await counterEl.waitFor({ state: 'visible', timeout: 10000 });

    const initialValue = await counterEl.innerText();
    await assert(initialValue.trim() === '0', `Home counter initial value is 0 (got: "${initialValue.trim()}")`);

    const clickButton = page.locator('button', { hasText: /click me/i });
    await clickButton.waitFor({ state: 'visible', timeout: 10000 });
    await clickButton.click();

    const afterClick = await counterEl.innerText();
    await assert(afterClick.trim() === '1', `Home counter increments to 1 (got: "${afterClick.trim()}")`);

    await page.close();
}

// --- API endpoint: dynamic JSON data ---
async function testApiEndpoint() {
    console.log('\n[Suite] API endpoint (/api/hello)');
    const page = await browser.newPage();

    // GET request
    const getResponse = await page.request.get(`${BASE_URL}/api/hello`);
    await assert(
        getResponse.status() >= 200 && getResponse.status() < 300,
        `GET /api/hello returns 2xx (got: ${getResponse.status()})`
    );

    const rawGet = await getResponse.text();
    await assert(
        rawGet.includes('Hello from MetaSSR API!'),
        `GET /api/hello contains expected message`
    );
    await assert(
        // timestamp is dynamic — just verify the field exists
        rawGet.includes('timestamp'),
        `GET /api/hello contains dynamic timestamp field`
    );

    // POST with dynamic name
    const postResponse = await page.request.post(`${BASE_URL}/api/hello`, {
        headers: { 'Content-Type': 'application/json' },
        data: JSON.stringify({ name: 'MetaSSR' }),
    });
    await assert(
        postResponse.status() >= 200 && postResponse.status() < 300,
        `POST /api/hello returns 2xx (got: ${postResponse.status()})`
    );
    const rawPost = await postResponse.text();
    await assert(
        rawPost.includes('MetaSSR'),
        `POST /api/hello echoes dynamic name "MetaSSR" (got: "${rawPost.substring(0, 80)}")`
    );

    await page.close();
}

runTests().catch((err) => {
    console.error('Unexpected error:', err);
    if (browser) browser.close().catch(() => {});
    process.exit(1);
});
