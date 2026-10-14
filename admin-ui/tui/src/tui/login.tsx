/* `losos login`: pin the host key the way SSH pins a host key, then take the
 * token and the key that signs for it.
 *
 *   1. the host key's fingerprint, in groups of four, and the question
 *      whether it matches the one the web UI's Access tokens pane shows;
 *   2. the token, typed without echo;
 *   3. which of the stored keys signs (skipped when there is one). */

import * as React from "react";
import { Box, Text, useApp, useInput } from "ink";
import { groupFingerprint } from "../pgp";
import { t } from "../lang";
import type { Theme } from "../theme";
import type { HostKind } from "../vault";
import { Logo } from "./Logo";
import { wrap, type Span } from "./model";
import { LineView, splitKeys, TextField, ThemeContext } from "./ui";

export interface KeyChoice {
  fingerprint: string;
  userIds: string[];
}

export interface LoginAnswer {
  token: string;
  keyFingerprint: string;
}

export interface LoginProps {
  theme: Theme;
  url: string;
  kind: HostKind;
  fingerprint: string;
  keys: KeyChoice[];
  /** Skip the question: the fingerprint was confirmed on the command line. */
  confirmed?: boolean;
  /** Preset token or key (from flags). */
  token?: string;
  keyFingerprint?: string;
  cols: number;
  onDone: (answer: LoginAnswer | null) => void;
}

type Step = "fingerprint" | "token" | "key" | "refused";

export function Login(props: LoginProps) {
  const app = useApp();
  const [step, setStep] = React.useState<Step>(props.confirmed === true ? "token" : "fingerprint");
  const [token, setToken] = React.useState(props.token ?? "");
  const [keyIndex, setKeyIndex] = React.useState(0);

  const finish = (answer: LoginAnswer | null): void => {
    props.onDone(answer);
    app.exit();
  };

  const afterToken = (value: string): void => {
    setToken(value);
    if (props.keyFingerprint !== undefined) finish({ token: value, keyFingerprint: props.keyFingerprint });
    else if (props.keys.length === 1) finish({ token: value, keyFingerprint: props.keys[0]!.fingerprint });
    else setStep("key");
  };

  React.useEffect(() => {
    if (step === "token" && props.token !== undefined) afterToken(props.token);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step]);

  // Keys read through refs: several can arrive in one chunk (see splitKeys).
  const stepRef = React.useRef(step);
  stepRef.current = step;
  const keyRef = React.useRef(keyIndex);
  keyRef.current = keyIndex;
  useInput(
    (raw, rawKey) => {
      for (const [input, key] of splitKeys(raw, rawKey)) {
        if (stepRef.current === "fingerprint") {
          if (input === "y" || input === "Y") {
            stepRef.current = "token";
            setStep("token");
          } else if (input === "n" || input === "N" || key.escape) {
            stepRef.current = "refused";
            setStep("refused");
            finish(null);
          }
          // The token field is not there yet; what follows in this chunk is dropped.
          return;
        }
        if (stepRef.current === "key") {
          if (key.upArrow || input === "k") keyRef.current = Math.max(0, keyRef.current - 1);
          else if (key.downArrow || input === "j") keyRef.current = Math.min(props.keys.length - 1, keyRef.current + 1);
          else if (key.return) {
            finish({ token, keyFingerprint: props.keys[keyRef.current]!.fingerprint });
            return;
          } else if (key.escape) {
            finish(null);
            return;
          }
          setKeyIndex(keyRef.current);
        }
      }
    },
    { isActive: step === "fingerprint" || step === "key" },
  );

  const theme = props.theme;
  const textCols = Math.max(30, props.cols - 40);
  const groups = groupFingerprint(props.fingerprint).split(" ");
  const lines: Span[][] = [
    [{ text: "LosOS", color: "ink", bold: true }, { text: `  ${t(props.kind === "edge" ? "tui.kind.edge" : "tui.kind.box")}`, color: "faint" }],
    [{ text: props.url, color: "muted" }],
    [],
    [{ text: t("tui.login.hostKey"), color: "muted", bold: true }],
    [{ text: groups.slice(0, 5).join(" "), color: "accent", bold: true }],
    [{ text: groups.slice(5).join(" "), color: "accent", bold: true }],
    [],
  ];
  const question = t(props.kind === "edge" ? "tui.login.compareEdge" : "tui.login.compareBox");
  for (const text of wrap(question, textCols)) lines.push([{ text, color: "ink" }]);
  lines.push([]);

  return (
    <ThemeContext.Provider value={theme}>
      <Box flexDirection="row" width={props.cols}>
        <Box flexDirection="column" flexShrink={0}>
          <Text backgroundColor={theme.c.ground}> </Text>
          <Logo size="large" theme={theme} />
        </Box>
        <Box flexDirection="column" marginLeft={0}>
          <Text backgroundColor={theme.c.ground}>{" ".repeat(textCols + 4)}</Text>
          {lines.map((spans, i) => (
            <LineView key={i} line={{ spans: [{ text: "   " }, ...spans], bg: "ground" }} cols={textCols + 4} />
          ))}
          {step === "fingerprint" && (
            <LineView
              line={{
                spans: [
                  { text: "   " },
                  { text: " y ", color: "ground", bg: "accent", bold: true },
                  { text: ` ${t("tui.login.matches")}   `, color: "ink" },
                  { text: " n ", color: "ink", bg: "sunk", bold: true },
                  { text: ` ${t("tui.login.differs")}`, color: "muted" },
                ],
                bg: "ground",
              }}
              cols={textCols + 4}
            />
          )}
          {step === "token" && (
            <Box flexDirection="column">
              <LineView line={{ spans: [{ text: "   " }, { text: t("tui.login.token"), color: "ink", bold: true }], bg: "ground" }} cols={textCols + 4} />
              <Box flexDirection="row">
                <Text backgroundColor={theme.c.ground}>{"   "}</Text>
                <TextField masked onSubmit={afterToken} onCancel={() => finish(null)} />
              </Box>
              <LineView line={{ spans: [{ text: "   " }, { text: t("tui.login.tokenHint"), color: "faint" }], bg: "ground" }} cols={textCols + 4} />
            </Box>
          )}
          {step === "key" && (
            <Box flexDirection="column">
              <LineView line={{ spans: [{ text: "   " }, { text: t("tui.login.whichKey"), color: "ink", bold: true }], bg: "ground" }} cols={textCols + 4} />
              {props.keys.map((k, i) => (
                <LineView
                  key={k.fingerprint}
                  line={{
                    spans: [
                      { text: i === keyIndex ? " › " : "   ", color: "accent", bold: true },
                      { text: groupFingerprint(k.fingerprint).slice(-24), color: i === keyIndex ? "accent" : "ink" },
                      { text: `  ${k.userIds[0] ?? ""}`, color: "muted" },
                    ],
                    bg: i === keyIndex ? "accentWash" : "ground",
                  }}
                  cols={textCols + 4}
                />
              ))}
            </Box>
          )}
        </Box>
      </Box>
    </ThemeContext.Provider>
  );
}

/** The host picker, when more than one host is pinned and none was named. */
export function Picker({
  theme,
  hosts,
  cols,
  onPick,
}: {
  theme: Theme;
  hosts: { url: string; kind: HostKind }[];
  cols: number;
  onPick: (url: string | null) => void;
}) {
  const app = useApp();
  const [index, setIndex] = React.useState(0);
  useInput((input, key) => {
    if (key.upArrow || input === "k") setIndex((i) => Math.max(0, i - 1));
    else if (key.downArrow || input === "j") setIndex((i) => Math.min(hosts.length - 1, i + 1));
    else if (key.return) {
      onPick(hosts[index]?.url ?? null);
      app.exit();
    } else if (input === "q" || key.escape) {
      onPick(null);
      app.exit();
    }
  });
  const width = Math.max(40, cols - 22);
  return (
    <ThemeContext.Provider value={theme}>
      <Box flexDirection="row" width={cols}>
        <Box flexDirection="column" flexShrink={0}>
          <Text backgroundColor={theme.c.ground}> </Text>
          <Logo size="small" theme={theme} />
        </Box>
        <Box flexDirection="column">
          <LineView line={{ spans: [{ text: "  " }], bg: "ground" }} cols={width} />
          <LineView line={{ spans: [{ text: "  LosOS", color: "ink", bold: true }], bg: "ground" }} cols={width} />
          <LineView line={{ spans: [{ text: `  ${t("tui.pick.title")}`, color: "muted" }], bg: "ground" }} cols={width} />
          <LineView line={{ spans: [], bg: "ground" }} cols={width} />
          {hosts.map((h, i) => (
            <LineView
              key={h.url}
              line={{
                spans: [
                  { text: i === index ? " › " : "   ", color: "accent", bold: true },
                  { text: h.url, color: i === index ? "accent" : "ink", bold: i === index },
                  { text: `  ${t(h.kind === "edge" ? "tui.kind.edge" : "tui.kind.box")}`, color: "faint" },
                ],
                bg: i === index ? "accentWash" : "ground",
              }}
              cols={width}
            />
          ))}
        </Box>
      </Box>
    </ThemeContext.Provider>
  );
}
