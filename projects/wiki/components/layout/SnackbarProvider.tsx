import { useMediaQuery } from '@mui/material';
import { drawerWidth } from 'core/constants';
import { SnackbarProvider as NoSnackbarProvider } from 'notistack';

const SnackbarProvider = ({ children }: { children: JSX.Element }) => {
  const largeScreen = useMediaQuery('(min-width:1200px)');
  return (
    <NoSnackbarProvider
      style={{
        marginLeft: largeScreen ? `${drawerWidth + 28}px` : '-12px',
        marginBottom: '50px',
      }}
      maxSnack={3}
      preventDuplicate
    >
      {children}
    </NoSnackbarProvider>
  );
};

export default SnackbarProvider;
