import {
  Event,
  Folder,
  Gavel,
  Group,
  HowToReg,
  Person,
  Subject,
  RateReview,
  Home,
  Poll,
  Face,
  Article,
  UploadFile,
  QuestionMark,
  InterpreterMode,
  Image,
  MusicNote,
  Search,
  Edit,
  LowPriority,
  HowToVote,
  RecordVoiceOver,
  AddComment,
  Map,
  ConnectedTv,
} from '@mui/icons-material';
import MicrosoftExcelIcon from './svg/microsoft-excel.svg';
import MicrosoftWordIcon from './svg/microsoft-word.svg';
import MicrosoftPowerpointIcon from './svg/microsoft-powerpoint.svg';
import FilePdfBoxIcon from './svg/file-pdf-box.svg';
import VideoBoxIcon from './svg/video-box.svg';
import { Avatar as MuiAvatar, Skeleton, Typography } from '@mui/material';
import { Maybe } from 'gql';
import { Box } from '@mui/system';

const getLetter = (index: number) => {
  const f = String.fromCharCode(65 + (index % 26));
  return index >= 26 ? String.fromCharCode(64 + Math.floor(index / 26)) + f : f;
};

const IconId = ({
  mimeId,
  index,
  name,
  avatar,
}: {
  mimeId: Maybe<string | undefined>;
  index?: number;
  name?: string;
  avatar?: boolean;
}) => {
  switch (mimeId) {
    case 'wiki/search':
      return <Search />;
    case 'wiki/home':
      return <Home />;
    case 'wiki/group':
      return <Group />;
    case 'wiki/event':
      return <Event />;
    case 'wiki/folder':
      return name?.[0] ? (
        <Box sx={{ color: 'inherit', minWidth: '25px' }}>
          <Folder
            sx={{
              fontSize: 32,
              position: avatar ? 'relative' : 'absolute',
              marginLeft: avatar ? '0px' : '-3px',
              marginTop: avatar ? '0px' : '-3px',
            }}
          />
          <Typography
            sx={{
              color: avatar ? 'secondary.main' : 'common.white',
              position: avatar ? 'absolute' : 'relative',
              top: avatar ? 8 : 3,
              left: avatar ? 16 : 8,
            }}
          >
            <b>{name?.[0]}</b>
          </Typography>
        </Box>
      ) : (
        <Folder />
      );
    case 'wiki/document':
      return <Article />;
    case 'wiki/file':
      return <UploadFile />;
    case 'wiki/user':
      return <Person />;
    case 'text/plain':
      return <Subject />;
    case 'vote/policy':
      return index !== undefined && index !== -1 ? (
        avatar ? (
          <Typography fontSize={24} sx={{ color: 'inherit' }}>
            {getLetter(index)}
          </Typography>
        ) : (
          <MuiAvatar
            sx={{
              width: 24,
              height: 24,
              color: 'currentColor',
              bgcolor: 'currentColor',
            }}
          >
            <Typography
              fontSize={18}
              sx={{
                color: avatar ? 'secondary.main' : 'common.white',
              }}
            >
              {getLetter(index)}
            </Typography>
          </MuiAvatar>
        )
      ) : (
        <Gavel />
      );
    case 'vote/position':
      return <HowToReg />;
    case 'vote/candidate':
      return <Face />;
    case 'vote/question':
      return <QuestionMark />;
    case 'vote/comment':
      return <AddComment />;
    case 'vote/change':
      return index !== undefined && index !== -1 ? (
        avatar ? (
          <Typography fontSize={24} sx={{ color: 'inherit' }}>
            {index + 1}
          </Typography>
        ) : (
          <MuiAvatar
            sx={{
              width: 24,
              height: 24,
              color: 'currentColor',
              bgcolor: 'currentColor',
            }}
          >
            <Typography
              fontSize={18}
              sx={{
                color: avatar ? 'secondary.main' : 'common.white',
              }}
            >
              {index + 1}
            </Typography>
          </MuiAvatar>
        )
      ) : (
        <RateReview />
      );
    case 'vote/poll':
      return <Poll />;
    case 'speak/list':
      return <InterpreterMode />;
    case 'application/pdf':
      return <FilePdfBoxIcon fill="currentColor" height="24" width="24" />;
    case 'app/home':
      return <Home />;
    case 'app/editor':
      return <Edit />;
    case 'app/sort':
      return <LowPriority />;
    case 'app/vote':
      return <HowToVote />;
    case 'app/speak':
      return <RecordVoiceOver />;
    case 'app/search':
      return <Search />;
    case 'app/member':
      return <Group />;
    case 'app/folder':
      return <Folder />;
    case 'app/screen':
      return <ConnectedTv />;
    case 'app/map':
    case 'map/map':
      return <Map />;
    case undefined:
      return <Skeleton variant="circular" width={24} height={24} />;
    default:
  }

  // eslint-disable-next-line jsx-a11y/alt-text
  if (mimeId?.includes('image/')) return <Image />;
  if (mimeId?.includes('audio/')) return <MusicNote />;
  if (mimeId?.includes('video/'))
    return <VideoBoxIcon fill="currentColor" height="24" width="24" />;
  if (mimeId?.includes('spreadsheet'))
    return <MicrosoftExcelIcon fill="currentColor" height="24" width="24" />;
  if (mimeId?.includes('presentation'))
    return (
      <MicrosoftPowerpointIcon fill="currentColor" height="24" width="24" />
    );
  if (mimeId?.includes('document'))
    return <MicrosoftWordIcon fill="currentColor" height="24" width="24" />;

  return <QuestionMark />;
};

const getName = (mimeId?: string): string => {
  switch (mimeId) {
    case 'wiki/group':
      return 'Gruppe';
    case 'wiki/event':
      return 'Begivenhed';
    case 'wiki/folder':
      return 'Mappe';
    case 'wiki/document':
      return 'Dokument';
    case 'wiki/file':
      return 'Fil';
    case 'wiki/user':
      return 'Person';
    case 'text/plain':
      return 'Dokument';
    case 'vote/policy':
      return 'Politik';
    case 'vote/position':
      return 'Post';
    case 'vote/change':
      return 'Ændringsforslag';
    case 'vote/candidate':
      return 'Kandidatur';
    case 'vote/question':
      return 'Spørgsmål';
    case 'vote/comment':
      return 'Kommentar';
    case 'speak/list':
      return 'Talerliste';
    case 'app/editor':
      return 'Rediger';
    case 'app/sort':
      return 'Sorter';
    case 'app/speak':
      return 'Tal';
    case 'app/vote':
      return 'Stem';
    case 'app/member':
      return 'Medlemmer';
    case 'app/map':
    case 'map/map':
      return 'Kort';
    default:
      return 'Ukendt';
  }
};

export { IconId, getLetter, getName };
