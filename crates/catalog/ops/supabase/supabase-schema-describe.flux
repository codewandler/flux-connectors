op supabase-schema-describe -> Any
  description "Describe the tables, views and columns this project's data API exposes, as an OpenAPI 2.0 document. Takes no argument. Call this first to discover the table names supabase-rows-list takes and what columns each one has — the document reflects only what the anon role may see, so a table absent from it is a table this key cannot read. Also this connector's `verify`: a bounded read that runs unattended and needs nothing configured beyond the project ref and the key"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{project_ref}.supabase.co"
  url = fmt("{base}/rest/v1/")
  response = http.request(method: "GET", url)
  return response
