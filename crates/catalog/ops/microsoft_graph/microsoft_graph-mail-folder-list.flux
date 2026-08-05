op microsoft_graph-mail-folder-list -> Any
  description "List the top-level mail folders in the signed-in user's mailbox — Inbox, Drafts, Sent Items, and any custom top-level folders. Does not descend into subfolders. Graph's default page size here is 10; this connector cannot follow `@odata.nextLink` (an absolute URL, not a constructible cursor), so a mailbox with more than ten top-level folders is reported incompletely. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/mailFolders")
  response = http.request(method: "GET", url)
  return response
