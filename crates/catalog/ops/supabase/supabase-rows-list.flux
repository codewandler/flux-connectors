op supabase-rows-list(table: String, limit: Number) -> Any
  description "Read rows from one table or view in this project. Returns a JSON array of row objects with every column the anon role may read; the columns are the project's own, so call supabase-schema-describe first to learn them. Row-level security decides which rows come back — an empty array means the policies matched nothing, not that the table is empty. This connector cannot filter, project or sort: pass `limit` to bound the read and do the rest in the flow"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{project_ref}.supabase.co"
  url = fmt("{base}/rest/v1/{table}")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
