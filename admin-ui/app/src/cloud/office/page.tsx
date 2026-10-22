/* A cloud document in LosOS Office, at /cloud/office/:file. The editor
 * takes the width and the height under the cloud's top bar, like a text
 * document at /cloud/edit/:file. */

import * as React from "react";
import { useNavigate, useParams } from "react-router-dom";
import { OfficeEditor } from "./editor";
import { openOffice } from "./api";

export function OfficePage() {
  const { file = "" } = useParams();
  const navigate = useNavigate();
  const load = React.useCallback(() => openOffice(file), [file]);
  // Back where the person came from; a document opened in a tab of its own
  // goes to the files.
  const close = React.useCallback(() => {
    if (window.history.state?.idx > 0) navigate(-1);
    else navigate("/files");
  }, [navigate]);
  return <OfficeEditor key={file} load={load} onClose={close} />;
}
