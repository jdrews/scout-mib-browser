describe('Scout MIB Browser', () => {
  before(async () => {
    await browser.pause(2000);
  });

  it('should display the app title', async () => {
    const title = await browser.getTitle();
    expect(title).toBe('Scout MIB Browser');
  });

  it('should show the footer element', async () => {
    const footer = await $('footer');
    await expect(footer).toBeExisting();
  });

  it('should show disconnected status indicator', async () => {
    const dot = await $('[class*="rounded-full"]');
    await expect(dot).toBeExisting();
  });

  it('should have the target bar visible', async () => {
    const targetBar = await $('[data-address-bar]');
    await expect(targetBar).toBeExisting();
  });
});
