import { useQuery } from "@tanstack/react-query"
import { api } from "@/lib/api"

/** Worker status; polls fast while something is happening. */
export function useStatus() {
  return useQuery({
    queryKey: ["status"],
    queryFn: api.status,
    refetchInterval: (q) =>
      q.state.data?.now === "idle" && !q.state.data.queued ? 15_000 : 2_000,
  })
}
