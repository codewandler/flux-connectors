op docusign-envelope-create-from-template(templateId: String, templateRoles: List<Any>, status: String) -> Any
  description "Create an envelope from an existing template. When status is sent, DocuSign dispatches it immediately to every named recipient — a real, legally binding signature request to a real person. Use status created to save it as an editable draft that notifies nobody instead. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/errorCode` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{account_host}/restapi/v2.1/accounts/{account_id}"
  url = fmt("{base}/envelopes")
  content_type = "application/json"
  payload = { status, templateId, templateRoles }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
