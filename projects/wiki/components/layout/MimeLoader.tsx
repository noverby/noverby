import { Node, useNode } from 'hooks';
import {
  FolderApp,
  ContentApp,
  GroupApp,
  PolicyApp,
  UserApp,
  PollApp,
  EventApp,
  PositionApp,
  HomeApp,
  CandidateApp,
  FileApp,
  MapApp,
} from 'comps';

const SelectApp = ({ mimeId, node }: { mimeId: string; node: Node }) => {
  switch (mimeId) {
    case 'wiki/folder':
      return <FolderApp node={node} />;
    case 'wiki/document':
      return <ContentApp node={node} />;
    case 'wiki/file':
      return <FileApp node={node} />;
    case 'wiki/group':
      return <GroupApp node={node} />;
    case 'wiki/event':
      return <EventApp node={node} />;
    case 'wiki/user':
      return <UserApp node={node} />;
    case 'vote/policy':
    case 'vote/change':
      return <PolicyApp node={node} />;
    case 'vote/position':
      return <PositionApp node={node} />;
    case 'vote/candidate':
      return <CandidateApp node={node} />;
    case 'vote/poll':
      return <PollApp node={node} />;
    case 'map/map':
      return <MapApp node={node} />;
    case 'wiki/home':
      return <HomeApp />;
    default:
      return null;
  }
};

const MimeLoader = (param?: { id?: string; mimeId?: string }) => {
  const node = useNode({ id: param?.id });
  
  if (!node?.mimeId && !param?.mimeId) return null;

  return <SelectApp mimeId={param?.mimeId ?? node.mimeId ?? ''} node={node} />;
};

export default MimeLoader;
