import React, {type ReactNode} from 'react';
import Logo from '@theme-original/Logo';
import type LogoType from '@theme/Logo';
import type {WrapperProps} from '@docusaurus/types';
import {translate} from '@docusaurus/Translate';

type Props = WrapperProps<typeof LogoType>;

/* The navbar's plate of salmon, as in the admin UI's top bar (App.tsx), with
 * the same word of explanation: the stock Logo forwards unknown props to its
 * link, so the explanation rides as the link's title, the browser's own
 * tooltip. (The SPA draws its tooltip itself; the handbook has no such
 * component and adds no dependency for one.) */
export default function LogoWrapper(props: Props): ReactNode {
  const tip = translate({
    id: 'losos.navbar.logoTip',
    message: "Losos is Slovak for salmon. The logo is a plate of it, from the owner's own photo.",
    description: 'The tooltip on the navbar logo, saying what the picture means',
  });
  return <Logo {...props} title={tip} />;
}
