from playwright.sync_api import sync_playwright

import time

def run(playwright):
    browser = playwright.chromium.launch()
    page = browser.new_page()
    time.sleep(5)  # Wait for the server to start
    page.goto("http://localhost:3000")
    page.wait_for_selector("#generation-status")
    page.screenshot(path="jules-scratch/verification/verification.png")
    browser.close()

with sync_playwright() as playwright:
    run(playwright)