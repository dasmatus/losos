import { Topbar } from 'losos-ds';

export const WithLinks = () => (
  <Topbar brand="losos" href="#">
    <a href="#nextcloud">Nextcloud</a>
    <a href="#forgejo">Forgejo</a>
    <a href="#settings">Settings</a>
  </Topbar>
);

export const Minimal = () => <Topbar brand="losos" href="#" />;
