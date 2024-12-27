import {
  EditorApp,
  SpeakApp,
  SortApp,
  VoteApp,
  ScreenApp,
  MemberApp,
  MapApp
} from 'comps';
import { useNode } from 'hooks';

const AppLoader = ({ app, id }: { app?: string; id?: string }) => {
  const node = useNode({ id });

  if (app) {
    switch (app) {
      case 'member':
        return <MemberApp node={node} />;
      //case "perm":
      //  return <PermApp node={node} />;
      case 'editor':
        return <EditorApp node={node} />;
      case 'speak':
        return <SpeakApp node={node} />;
      case 'sort':
        return <SortApp node={node} />;
      case 'vote':
        return <VoteApp node={node} />;
      case 'screen':
        return <ScreenApp node={node} />;
      case 'map':
        return <MapApp node={node} />;
      default:
    }
  }

  return null;
};

export default AppLoader;
