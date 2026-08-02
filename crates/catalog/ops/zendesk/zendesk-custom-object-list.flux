op zendesk-custom-object-list(include_ui_path: Bool) -> Any
  description "List custom-object definitions, optionally including each definition's UI path"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/custom_objects")
  sep = "?"
  when include_ui_path
    url = fmt("{url}{sep}include_ui_path={include_ui_path}")
  response = http.request(method: "GET", url)
  return response
