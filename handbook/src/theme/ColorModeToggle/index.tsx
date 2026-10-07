import React, {type ReactNode} from 'react';
import clsx from 'clsx';
import useIsBrowser from '@docusaurus/useIsBrowser';
import {translate} from '@docusaurus/Translate';
import type {Props} from '@theme/ColorModeToggle';

/* The admin UI's ThemeToggle (admin-ui/app/src/components/ui/theme-toggle.tsx):
 * a three-way segmented control, Auto / Light / Dark, rather than the stock
 * one-button cycle. Same markup as the SPA's shadcn Toggle Group (a
 * radiogroup of role="radio" buttons, one Tab stop, arrows move the choice)
 * and the same three Hugeicons glyphs, inlined so the handbook adds no icon
 * dependency. Which segment is "on" is drawn from the html element's
 * data-theme-choice (set by Docusaurus before React runs), so the control
 * is right before hydration and the server HTML needs no state. */

type Choice = Props['value'];

const ICONS: Record<'system' | 'light' | 'dark', ReactNode> = {
  system: (
    <>
      <path d="M14 21H16M14 21C13.1716 21 12.5 20.3284 12.5 19.5V17L12 17M14 21H10M10 21H8M10 21C10.8284 21 11.5 20.3284 11.5 19.5V17L12 17M12 17V21" />
      <path d="M16 3H8C5.17157 3 3.75736 3 2.87868 3.87868C2 4.75736 2 6.17157 2 9V11C2 13.8284 2 15.2426 2.87868 16.1213C3.75736 17 5.17157 17 8 17H16C18.8284 17 20.2426 17 21.1213 16.1213C22 15.2426 22 13.8284 22 11V9C22 6.17157 22 4.75736 21.1213 3.87868C20.2426 3 18.8284 3 16 3Z" />
    </>
  ),
  light: (
    <>
      <path d="M17 12C17 14.7614 14.7614 17 12 17C9.23858 17 7 14.7614 7 12C7 9.23858 9.23858 7 12 7C14.7614 7 17 9.23858 17 12Z" />
      <path d="M12 2V3.5M12 20.5V22M19.0708 19.0713L18.0101 18.0106M5.98926 5.98926L4.9286 4.9286M22 12H20.5M3.5 12H2M19.0713 4.92871L18.0106 5.98937M5.98975 18.0107L4.92909 19.0714" />
    </>
  ),
  dark: (
    <path d="M21.5 14.0784C20.3003 14.7189 18.9301 15.0821 17.4751 15.0821C12.7491 15.0821 8.91792 11.2509 8.91792 6.52485C8.91792 5.06986 9.28105 3.69968 9.92163 2.5C5.66765 3.49698 2.5 7.31513 2.5 11.8731C2.5 17.1899 6.8101 21.5 12.1269 21.5C16.6849 21.5 20.503 18.3324 21.5 14.0784Z" />
  ),
};

function label(choice: Choice): string {
  switch (choice) {
    case null:
      return translate({
        message: 'system mode',
        id: 'theme.colorToggle.ariaLabel.mode.system',
        description: 'The name for the system color mode',
      });
    case 'light':
      return translate({
        message: 'light mode',
        id: 'theme.colorToggle.ariaLabel.mode.light',
        description: 'The name for the light color mode',
      });
    case 'dark':
      return translate({
        message: 'dark mode',
        id: 'theme.colorToggle.ariaLabel.mode.dark',
        description: 'The name for the dark color mode',
      });
    default:
      throw new Error(`unexpected color mode ${String(choice)}`);
  }
}

function ColorModeToggle({
  className,
  respectPrefersColorScheme,
  value,
  onChange,
}: Props): ReactNode {
  const isBrowser = useIsBrowser();
  const choices: Choice[] = respectPrefersColorScheme
    ? [null, 'light', 'dark']
    : ['light', 'dark'];

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
    const keys = ['ArrowRight', 'ArrowDown', 'ArrowLeft', 'ArrowUp', 'Home', 'End'];
    if (!keys.includes(event.key)) return;
    const items = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="radio"]'),
    );
    const at = items.findIndex((item) => item === document.activeElement);
    let next: number;
    if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = items.length - 1;
    else if (event.key === 'ArrowRight' || event.key === 'ArrowDown')
      next = at < 0 ? 0 : (at + 1) % items.length;
    else next = at <= 0 ? items.length - 1 : at - 1;
    event.preventDefault();
    const target = items[next];
    if (target === undefined) return;
    target.focus();
    const chosen = target.dataset['choice'];
    onChange(chosen === 'system' || chosen === undefined ? null : (chosen as Choice));
  };

  return (
    <div
      role="radiogroup"
      aria-label={translate({
        message: 'Appearance',
        id: 'theme.colorToggle.ariaLabel.group',
        description: 'The label of the colour-mode segmented control',
      })}
      className={clsx('hb-theme', className)}
      onKeyDown={onKeyDown}
    >
      {choices.map((choice) => {
        const key = choice ?? 'system';
        const on = value === choice;
        return (
          <button
            key={key}
            type="button"
            role="radio"
            data-choice={key}
            aria-checked={isBrowser ? on : undefined}
            tabIndex={isBrowser ? (on ? 0 : -1) : -1}
            disabled={!isBrowser}
            title={label(choice)}
            aria-label={label(choice)}
            className="hb-theme__item"
            onClick={() => onChange(choice)}
          >
            <svg
              aria-hidden="true"
              width={16}
              height={16}
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={1.5}
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              {ICONS[key]}
            </svg>
          </button>
        );
      })}
    </div>
  );
}

export default React.memo(ColorModeToggle);
