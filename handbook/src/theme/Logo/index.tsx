import React, {useEffect, type ReactNode} from 'react';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import {useThemeConfig} from '@docusaurus/theme-common';
import type {Props} from '@theme/Logo';
import {logoTip, useHalloween} from '@site/src/lib/logo';

/* The navbar's logo, as in the admin UI's top bar (App.tsx). It is the live
 * salmon, or the plate of salmon on Halloween, and the link's title carries
 * the same explanation, so the browser shows it as a tooltip. The SPA draws
 * its tooltip itself, and the handbook adds no dependency for one. This
 * replaces the stock Logo rather than wrapping it, because the stock one
 * takes its picture from the config and has no prop to change it by date.
 * On Halloween the tab's icon follows. */
export default function Logo(props: Props): ReactNode {
  const {
    navbar: {title, logo},
  } = useThemeConfig();
  const {imageClassName, titleClassName, ...rest} = props;
  const halloween = useHalloween();
  const salmon = useBaseUrl(logo?.src ?? 'img/losos.png');
  const plate = useBaseUrl('img/losos-halloween.png');
  const home = useBaseUrl(logo?.href ?? '/');
  const src = halloween ? plate : salmon;
  useEffect(() => {
    if (!halloween) return;
    document
      .querySelectorAll<HTMLLinkElement>('link[rel="icon"]')
      .forEach((link) => (link.href = plate));
  }, [halloween, plate]);
  const image = <img src={src} alt={logo?.alt ?? ''} className={logo?.className} />;
  return (
    <Link to={home} {...rest} title={logoTip(halloween)}>
      {imageClassName ? <div className={imageClassName}>{image}</div> : image}
      {title != null && <b className={titleClassName}>{title}</b>}
    </Link>
  );
}
