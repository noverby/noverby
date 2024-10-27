import { useRouter } from "next/router";

export default function Yt() {
  const router = useRouter();
  const regex = /.*v=([a-zA-Z0-9_-]{11}).*/;
  const res = router.asPath.match(regex);
  if (!res?.[1]) return null;

  return (
    <div style={{ position: 'relative', overflow: 'hidden', width: '100%', height: '100vh' }}>
      <iframe width="100%" height="100%" src={`https://youtube.com/embed/${res[1]}?enablejsapi=1`}
        frameBorder="0"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
        allowFullScreen />
    </div>
  );
}
