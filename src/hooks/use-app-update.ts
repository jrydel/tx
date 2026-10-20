import { useMutation, useQuery } from "@tanstack/react-query"
import { getVersion } from "@tauri-apps/api/app"
import { relaunch } from "@tauri-apps/plugin-process"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { toast } from "sonner"

export function useAppVersion() {
  return useQuery({
    queryKey: ["app-version"],
    queryFn: getVersion,
    staleTime: Infinity,
  })
}

/** Checks GitHub Releases on start and every 6 hours; `install` downloads, installs and restarts. */
export function useAppUpdate() {
  const update = useQuery({
    queryKey: ["app-update"],
    queryFn: async (): Promise<Update | null> => check(),
    refetchInterval: 6 * 60 * 60 * 1000,
    refetchOnWindowFocus: false,
    retry: false,
  })

  const install = useMutation({
    mutationFn: async (u: Update) => {
      const id = toast.loading(`Downloading tx ${u.version}…`)
      let total = 0
      let done = 0
      try {
        await u.downloadAndInstall((e) => {
          if (e.event === "Started") total = e.data.contentLength ?? 0
          if (e.event === "Progress") {
            done += e.data.chunkLength
            if (total)
              toast.loading(
                `Downloading tx ${u.version}… ${Math.round((done / total) * 100)}%`,
                { id }
              )
          }
          if (e.event === "Finished")
            toast.loading(`Installing tx ${u.version}…`, { id })
        })
        toast.success(`tx ${u.version} installed. Restarting…`, { id })
      } catch (err) {
        toast.dismiss(id)
        throw err
      }
      await relaunch()
    },
    onError: (e) => toast.error(`Update failed: ${String(e)}`),
  })

  return { update, install }
}
