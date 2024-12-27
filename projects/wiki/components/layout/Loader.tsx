import { useSearchParams } from 'next/navigation';
import { MimeLoader, AppLoader, HomeApp, UnknownApp } from 'comps';

const Loader = ({ app, id }: { app?: string; id?: string }) => {
  const params = useSearchParams();

  if (!params.get("app") && app === 'home') {
    return <HomeApp />;
  } else if (params.get("app") || app) {
    return (
      (id && <AppLoader app={params.get("app") ?? app} id={id} />) ||
      null
    );
  } else {
    return id ? <MimeLoader id={id} /> : <UnknownApp />;
  }
};

export default Loader;
