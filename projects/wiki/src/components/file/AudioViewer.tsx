const AudioViewer = ({ file }: { file?: string }) =>
	file ? (
		<audio autoPlay controls>
			<source src={file} />
		</audio>
	) : null;

export default AudioViewer;
